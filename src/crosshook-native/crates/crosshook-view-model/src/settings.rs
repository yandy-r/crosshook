//! Settings form worked example: one server-validated text field, one
//! client-validated numeric field and one select, driven by [`Form`].
//!
//! - `profiles_directory` is trimmed into the request; empty means the
//!   default directory. Its errors come from the backend
//!   (`Validation { field: "profiles_directory" }`).
//! - `recent_files_limit` is numeric text; the client check requires an
//!   integer in `1..=100`. This is a UI-input check only: core still clamps
//!   the value server-side.
//! - `log_filter` is a select over [`LogFilter`].

use crate::error::ServiceError;
use crate::form::{Form, FormModel, SubmitId};
use crate::ViewModel;

/// Message shown when `recent_files_limit` is not an integer in `1..=100`.
pub const RECENT_FILES_LIMIT_MESSAGE: &str = "Enter a whole number from 1 to 100.";

/// Log verbosity choices of the settings select.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogFilter {
    /// `error`
    Error,
    /// `warn`
    Warn,
    /// `info`
    Info,
    /// `debug`
    Debug,
    /// `trace`
    Trace,
}

impl LogFilter {
    /// Every choice, in display order.
    pub const ALL: &'static [Self] = &[
        Self::Error,
        Self::Warn,
        Self::Info,
        Self::Debug,
        Self::Trace,
    ];

    /// Backend value (`log_filter` setting string).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

/// Editable settings values, as the inputs hold them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsFields {
    /// Profiles directory text; trimmed on save, empty means the default.
    pub profiles_directory: String,
    /// Recent files limit as typed; must parse to an integer in `1..=100`.
    pub recent_files_limit: String,
    /// Selected log filter.
    pub log_filter: LogFilter,
}

/// Fields of [`SettingsFields`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SettingsField {
    /// Backend id `"profiles_directory"`.
    ProfilesDirectory,
    /// Backend id `"recent_files_limit"`.
    RecentFilesLimit,
    /// Backend id `"log_filter"`.
    LogFilter,
}

impl FormModel for SettingsFields {
    type Field = SettingsField;

    const FIELDS: &'static [SettingsField] = &[
        SettingsField::ProfilesDirectory,
        SettingsField::RecentFilesLimit,
        SettingsField::LogFilter,
    ];

    fn field_id(field: SettingsField) -> &'static str {
        match field {
            SettingsField::ProfilesDirectory => "profiles_directory",
            SettingsField::RecentFilesLimit => "recent_files_limit",
            SettingsField::LogFilter => "log_filter",
        }
    }

    fn field_changed(&self, other: &Self, field: SettingsField) -> bool {
        match field {
            SettingsField::ProfilesDirectory => self.profiles_directory != other.profiles_directory,
            SettingsField::RecentFilesLimit => self.recent_files_limit != other.recent_files_limit,
            SettingsField::LogFilter => self.log_filter != other.log_filter,
        }
    }

    fn validate(&self) -> Vec<(SettingsField, &'static str)> {
        match parse_recent_files_limit(&self.recent_files_limit) {
            Some(_) => Vec::new(),
            None => vec![(SettingsField::RecentFilesLimit, RECENT_FILES_LIMIT_MESSAGE)],
        }
    }
}

/// Parsed values the save effect carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsSaveRequest {
    /// Trimmed profiles directory; empty means the default.
    pub profiles_directory: String,
    /// Recent files limit in `1..=100`.
    pub recent_files_limit: u32,
    /// Selected log filter.
    pub log_filter: LogFilter,
}

impl SettingsSaveRequest {
    /// Parses `fields`; `None` when they fail [`FormModel::validate`].
    pub fn from_fields(fields: &SettingsFields) -> Option<Self> {
        Some(Self {
            profiles_directory: fields.profiles_directory.trim().to_owned(),
            recent_files_limit: parse_recent_files_limit(&fields.recent_files_limit)?,
            log_filter: fields.log_filter,
        })
    }
}

fn parse_recent_files_limit(text: &str) -> Option<u32> {
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|limit| (1..=100).contains(limit))
}

/// User actions and service results of the settings screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsIntent {
    /// Settings (re)loaded from the backend; resets the form to them.
    Loaded(SettingsFields),
    /// Profiles directory input changed.
    EditProfilesDirectory(String),
    /// Recent files limit input changed.
    EditRecentFilesLimit(String),
    /// Log filter selection changed.
    EditLogFilter(LogFilter),
    /// A field lost focus.
    Blur(SettingsField),
    /// The user pressed Save.
    Submit,
    /// Result of the [`SettingsEffect::Save`] with `id`.
    SaveFinished {
        /// Id from the effect.
        id: SubmitId,
        /// Service result.
        result: Result<(), ServiceError>,
    },
}

/// Service calls the settings screen requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsEffect {
    /// Save `request`; report back with [`SettingsIntent::SaveFinished`].
    Save {
        /// Submit attempt id to echo back.
        id: SubmitId,
        /// Parsed values to save.
        request: SettingsSaveRequest,
    },
}

/// Settings screen view-model.
#[derive(Debug, Clone)]
pub struct SettingsViewModel {
    form: Form<SettingsFields>,
}

impl SettingsViewModel {
    /// Starts a pristine form over `fields`.
    pub fn new(fields: SettingsFields) -> Self {
        Self {
            form: Form::new(fields),
        }
    }

    /// Form state for the view (values, dirty, errors, submit state).
    pub fn form(&self) -> &Form<SettingsFields> {
        &self.form
    }
}

impl ViewModel for SettingsViewModel {
    type Intent = SettingsIntent;
    type Effect = SettingsEffect;

    fn reduce(&mut self, intent: SettingsIntent) -> Vec<SettingsEffect> {
        match intent {
            SettingsIntent::Loaded(fields) => self.form.reset(fields),
            SettingsIntent::EditProfilesDirectory(text) => {
                self.form.edit(SettingsField::ProfilesDirectory, |v| {
                    v.profiles_directory = text;
                });
            }
            SettingsIntent::EditRecentFilesLimit(text) => {
                self.form.edit(SettingsField::RecentFilesLimit, |v| {
                    v.recent_files_limit = text;
                });
            }
            SettingsIntent::EditLogFilter(filter) => {
                self.form.edit(SettingsField::LogFilter, |v| {
                    v.log_filter = filter;
                });
            }
            SettingsIntent::Blur(field) => self.form.touch(field),
            SettingsIntent::Submit => {
                if let Some((id, snapshot)) = self.form.begin_submit() {
                    let request = SettingsSaveRequest::from_fields(&snapshot)
                        .expect("begin_submit only returns snapshots that pass validate");
                    return vec![SettingsEffect::Save { id, request }];
                }
            }
            SettingsIntent::SaveFinished { id, result } => {
                self.form.finish_submit(id, result.as_ref().copied());
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests;
