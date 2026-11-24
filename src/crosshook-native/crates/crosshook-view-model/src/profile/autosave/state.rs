//! State, scope and section types for [`super::ProfileAutosave`].

use std::collections::BTreeSet;
use std::time::Duration;

use crate::launch::LaunchMethod;
use crate::ServiceError;

/// Monotonic timestamp (milliseconds since an arbitrary epoch) injected into
/// [`super::ProfileAutosave::apply`].
///
/// Using a value type instead of `std::time::Instant` keeps tests deterministic
/// without reading the system clock (which `clippy.toml` bans).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct AutosaveInstant(u64);

impl AutosaveInstant {
    /// Creates an instant from monotonic milliseconds.
    #[must_use]
    pub const fn from_millis(millis: u64) -> Self {
        Self(millis)
    }

    /// Underlying monotonic milliseconds.
    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0
    }

    /// Adds `duration`, saturating on overflow and truncating sub-millisecond
    /// precision.
    #[must_use]
    pub fn saturating_add(self, duration: Duration) -> Self {
        let delta = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Self(self.0.saturating_add(delta))
    }

    /// Elapsed duration since `earlier`, or [`Duration::ZERO`] when `earlier`
    /// is not earlier than `self`.
    #[must_use]
    pub const fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0))
    }
}

/// Revision hash identifying a serialized or normalized profile snapshot.
///
/// Used both to detect no-op edits back to the saved baseline and to suppress
/// self-originated `ProfilesChanged` events.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct RevisionHash(String);

impl RevisionHash {
    /// Wraps a revision hash string.
    #[must_use]
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    /// Returns the revision hash string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RevisionHash {
    fn from(hash: &str) -> Self {
        Self::new(hash)
    }
}

impl From<String> for RevisionHash {
    fn from(hash: String) -> Self {
        Self(hash)
    }
}

/// Identifier of one dispatched save request.
///
/// Unique across [`super::ProfileAutosave`] instances in the process (each
/// instance owns a distinct `mount`), so replies for an earlier editor instance
/// never match a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SaveId {
    pub(super) mount: u64,
    pub(super) seq: u64,
}

impl SaveId {
    /// Instance id that issued this save request.
    #[must_use]
    pub const fn mount(self) -> u64 {
        self.mount
    }

    /// Per-instance sequence number of this save request.
    #[must_use]
    pub const fn seq(self) -> u64 {
        self.seq
    }
}

/// Logical profile editor sections tracked for debouncing and save grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum AutosaveSection {
    /// Core profile fields edited in Hero Detail Profiles (`useHeroProfilesAutosave`).
    ProfileDraft,
    /// Pre-launch and post-exit hooks (`useHeroLaunchHooksAutosave`).
    LaunchHooks,
    /// Trainer and DLL injection fields (`useHeroTrainerAutosave`).
    Trainer,
    /// Custom environment variables (`useLaunchEnvironmentAutosave`, 400 ms blur debounce).
    Environment,
    /// Enabled launch optimizations (`profile_save_launch_optimizations`).
    LaunchOptimizations,
    /// Game gamescope settings (`profile_save_gamescope_config`).
    Gamescope,
    /// Trainer gamescope settings (`profile_save_trainer_gamescope_config`).
    TrainerGamescope,
    /// MangoHud overlay settings (`profile_save_mangohud_config`).
    #[cfg_attr(test, serde(rename = "mangohud"))]
    MangoHud,
    /// Curated and custom command arguments (`profile_save_command_arguments`).
    CommandArguments,
}

impl AutosaveSection {
    /// Every autosave section in canonical order.
    pub const ALL: &'static [Self] = &[
        Self::ProfileDraft,
        Self::LaunchHooks,
        Self::Trainer,
        Self::Environment,
        Self::LaunchOptimizations,
        Self::Gamescope,
        Self::TrainerGamescope,
        Self::MangoHud,
        Self::CommandArguments,
    ];

    /// Backend persistence target this section maps to on its own.
    #[must_use]
    pub const fn save_target(self) -> SaveTarget {
        match self {
            Self::ProfileDraft | Self::LaunchHooks | Self::Trainer | Self::Environment => {
                SaveTarget::FullProfile
            }
            Self::LaunchOptimizations => SaveTarget::Section(SectionSaveKind::LaunchOptimizations),
            Self::Gamescope => SaveTarget::Section(SectionSaveKind::Gamescope),
            Self::TrainerGamescope => SaveTarget::Section(SectionSaveKind::TrainerGamescope),
            Self::MangoHud => SaveTarget::Section(SectionSaveKind::MangoHud),
            Self::CommandArguments => SaveTarget::Section(SectionSaveKind::CommandArguments),
        }
    }

    /// Whether edits to this section only autosave for `proton_run` or
    /// `steam_applaunch` profiles (`useProfileLaunchAutosaveEffects.ts`).
    #[must_use]
    pub const fn requires_proton_or_steam(self) -> bool {
        matches!(self, Self::LaunchOptimizations | Self::CommandArguments)
    }
}

/// Launch subsection persisted via a dedicated `ProfileWriter` command.
///
/// Using these dedicated saves keeps `ProfileWriter` from invalidating the
/// cached health snapshot (see `CLAUDE.md` § *Profile writes*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum SectionSaveKind {
    /// `ProfileWriter::save_launch_optimizations`.
    LaunchOptimizations,
    /// `ProfileWriter::save_gamescope_config`.
    Gamescope,
    /// `ProfileWriter::save_trainer_gamescope_config`.
    TrainerGamescope,
    /// `ProfileWriter::save_mangohud_config`.
    #[cfg_attr(test, serde(rename = "mangohud"))]
    MangoHud,
    /// `ProfileWriter::save_command_arguments`.
    CommandArguments,
}

/// Which `ProfileWriter` path a [`super::SaveRequest`] should execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum SaveTarget {
    /// Full profile TOML write (`ProfileWriter::save_profile`), normalized first.
    FullProfile,
    /// Dedicated launch subsection write without health invalidation.
    Section(SectionSaveKind),
}

/// Active profile selection and save eligibility context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutosaveScope {
    /// Currently selected saved profile name (`selectedProfile` in Classic).
    pub selected_profile: String,
    /// Current draft name in the editor (`profileName` in Classic).
    pub draft_name: String,
    /// Whether `selected_profile` exists on disk.
    pub profile_exists: bool,
    /// Resolved launch method of the current draft.
    pub launch_method: LaunchMethod,
}

impl AutosaveScope {
    /// Convenience scope for an existing saved profile with matching draft name.
    #[must_use]
    pub fn saved(name: impl Into<String>, launch_method: LaunchMethod) -> Self {
        let name = name.into();
        Self {
            selected_profile: name.clone(),
            draft_name: name,
            profile_exists: true,
            launch_method,
        }
    }

    /// Convenience scope for an unsaved draft profile.
    #[must_use]
    pub fn unsaved(draft_name: impl Into<String>, launch_method: LaunchMethod) -> Self {
        Self {
            selected_profile: String::new(),
            draft_name: draft_name.into(),
            profile_exists: false,
            launch_method,
        }
    }

    /// Trimmed selected profile name (`""` when none).
    #[must_use]
    pub fn profile_name(&self) -> &str {
        self.selected_profile.trim()
    }

    /// True when the profile exists on disk and the draft name has not diverged
    /// (pauses autosave during unsaved creation and in-progress rename).
    #[must_use]
    pub fn can_autosave(&self) -> bool {
        let selected = self.selected_profile.trim();
        let draft = self.draft_name.trim();
        self.profile_exists && !selected.is_empty() && selected == draft
    }

    /// True when `launch_method` supports autosaving `section`, independent of
    /// profile existence or rename state.
    #[must_use]
    pub const fn method_supports_section(&self, section: AutosaveSection) -> bool {
        if section.requires_proton_or_steam() {
            return matches!(
                self.launch_method,
                LaunchMethod::ProtonRun | LaunchMethod::SteamApplaunch
            );
        }
        true
    }

    /// True when `section` is eligible to autosave under the current scope.
    #[must_use]
    pub fn can_autosave_section(&self, section: AutosaveSection) -> bool {
        self.can_autosave() && self.method_supports_section(section)
    }
}

impl Default for AutosaveScope {
    fn default() -> Self {
        Self {
            selected_profile: String::new(),
            draft_name: String::new(),
            profile_exists: false,
            launch_method: LaunchMethod::ProtonRun,
        }
    }
}

/// Observable state of [`super::ProfileAutosave`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AutosaveState {
    /// Clean; no pending timers, no active saves.
    #[default]
    Idle,
    /// Local edits are queued behind debounce / max-wait timers.
    Dirty {
        /// Monotonic time of the earliest unpersisted edit in this burst.
        since: AutosaveInstant,
        /// Sections with unpersisted edits.
        sections: BTreeSet<AutosaveSection>,
    },
    /// One save request is currently in flight.
    Saving {
        /// Request id echoed by `SaveSucceeded` / `SaveFailed`.
        id: SaveId,
        /// Persistence target (`FullProfile` or dedicated `Section`).
        target: SaveTarget,
        /// Sections covered by the in-flight request.
        sections: BTreeSet<AutosaveSection>,
        /// Draft revision sent in the save request.
        rev: RevisionHash,
    },
    /// All edits persisted; holding `"Profile saved"` briefly before `Idle`.
    Saved {
        /// Monotonic time the save completed.
        at: AutosaveInstant,
        /// Revision committed on disk.
        rev: RevisionHash,
    },
    /// The last save attempt failed; local edits remain dirty.
    Error {
        /// 1-based failure count since the last clean state or user reset.
        attempt: u32,
        /// Scheduled next automatic retry time, or `None` when retries are
        /// exhausted or the error is not retryable.
        retry_at: Option<AutosaveInstant>,
        /// Last error returned by the bridge.
        error: ServiceError,
    },
    /// External disk change collided with local unsaved or in-flight edits.
    Conflict {
        /// Latest local draft revision.
        local: RevisionHash,
        /// Remote revision reported with the external change, if available.
        remote: Option<RevisionHash>,
        /// Sections that held local edits when the conflict occurred.
        sections: BTreeSet<AutosaveSection>,
    },
}
