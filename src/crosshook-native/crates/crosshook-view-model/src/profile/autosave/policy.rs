//! Timing, retry policy and status chip types for [`super::ProfileAutosave`].

use std::time::Duration;

use super::state::AutosaveSection;

/// Default debounce delay for profile draft and launch subsection writes
/// (`src/hooks/profile/constants.ts::launchOptimizationsAutosaveDelayMs`).
pub const DEFAULT_SECTION_DEBOUNCE: Duration = Duration::from_millis(350);

/// Default blur-autosave debounce delay for environment variables
/// (`src/hooks/profile/useLaunchEnvironmentAutosave.ts:131`).
pub const DEFAULT_ENVIRONMENT_DEBOUNCE: Duration = Duration::from_millis(400);

/// Default duration the "Saved" chip stays visible before returning to `Idle`
/// (`src/components/launch-subtabs/useAutoSaveChip.ts:46`).
pub const DEFAULT_SAVED_STATUS_HOLD: Duration = Duration::from_millis(3_000);

/// Default maximum wait before continuous edits to the same section must flush.
///
/// Prevents unbounded starvation under rapid typing while leaving Classic
/// parity opt-in via [`AutosavePolicy::classic_parity`].
pub const DEFAULT_MAX_WAIT: Duration = Duration::from_millis(2_000);

/// Default retry backoff ladder (milliseconds) for transient save failures.
pub const DEFAULT_RETRY_BACKOFF_MS: &[u64] = &[1_000, 2_000, 4_000, 8_000];

/// Timing and retry configuration for [`super::ProfileAutosave`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutosavePolicy {
    /// Debounce delay for standard profile and launch sections (default 350 ms).
    pub section_debounce: Duration,
    /// Debounce delay for environment variable blur saves (default 400 ms).
    pub environment_debounce: Duration,
    /// Maximum wait from the first dirty edit in a burst (default 2,000 ms;
    /// `None` disables the cap).
    pub max_wait: Option<Duration>,
    /// Retry backoff schedule on retryable errors (default 1s, 2s, 4s, 8s;
    /// empty disables automatic retries).
    pub retry_backoff: Vec<Duration>,
    /// How long the `"Saved"` status holds before transitioning back to
    /// `Idle` (default 3,000 ms; `None` transitions straight to `Idle`).
    pub saved_status_hold: Option<Duration>,
}

impl AutosavePolicy {
    /// Policy matching Classic React's exact timers (350 ms / 400 ms / 3,000 ms)
    /// without a max-wait cap or automatic retry schedule.
    #[must_use]
    pub fn classic_parity() -> Self {
        Self {
            section_debounce: DEFAULT_SECTION_DEBOUNCE,
            environment_debounce: DEFAULT_ENVIRONMENT_DEBOUNCE,
            max_wait: None,
            retry_backoff: Vec::new(),
            saved_status_hold: Some(DEFAULT_SAVED_STATUS_HOLD),
        }
    }

    /// Debounce duration applied to edits of `section`.
    #[must_use]
    pub const fn debounce_for(&self, section: AutosaveSection) -> Duration {
        match section {
            AutosaveSection::Environment => self.environment_debounce,
            AutosaveSection::ProfileDraft
            | AutosaveSection::LaunchHooks
            | AutosaveSection::Trainer
            | AutosaveSection::LaunchOptimizations
            | AutosaveSection::Gamescope
            | AutosaveSection::TrainerGamescope
            | AutosaveSection::MangoHud
            | AutosaveSection::CommandArguments => self.section_debounce,
        }
    }

    /// Retry delay for 1-based failure `attempt`, or `None` once exhausted.
    #[must_use]
    pub fn retry_delay(&self, attempt: u32) -> Option<Duration> {
        let index = attempt.checked_sub(1)? as usize;
        self.retry_backoff.get(index).copied()
    }
}

impl Default for AutosavePolicy {
    fn default() -> Self {
        Self {
            section_debounce: DEFAULT_SECTION_DEBOUNCE,
            environment_debounce: DEFAULT_ENVIRONMENT_DEBOUNCE,
            max_wait: Some(DEFAULT_MAX_WAIT),
            retry_backoff: DEFAULT_RETRY_BACKOFF_MS
                .iter()
                .copied()
                .map(Duration::from_millis)
                .collect(),
            saved_status_hold: Some(DEFAULT_SAVED_STATUS_HOLD),
        }
    }
}

/// Visual tone of the autosave status chip, ordered by merge priority
/// (`useAutoSaveChip.ts:4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum AutosaveTone {
    /// No pending or recent autosave activity.
    #[default]
    Idle = 0,
    /// Save succeeded.
    Success = 1,
    /// Gated or conflict warning.
    Warning = 2,
    /// Debounce pending or write in flight.
    Saving = 3,
    /// Write failed.
    Error = 4,
}

impl AutosaveTone {
    /// Priority rank matching Classic `TONE_PRIORITY` (`0..=4`).
    #[must_use]
    pub const fn priority(self) -> u8 {
        self as u8
    }
}

/// Status badge payload shown by the autosave chip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutosaveStatus {
    /// Visual tone and chip priority.
    pub tone: AutosaveTone,
    /// Short human-readable status label.
    pub label: &'static str,
    /// Optional detail or error message.
    pub detail: Option<String>,
}

impl AutosaveStatus {
    /// Idle status (`tone: Idle`).
    #[must_use]
    pub const fn idle() -> Self {
        Self {
            tone: AutosaveTone::Idle,
            label: "Saved",
            detail: None,
        }
    }

    /// Pending or in-flight save status (`tone: Saving`).
    #[must_use]
    pub const fn saving() -> Self {
        Self {
            tone: AutosaveTone::Saving,
            label: "Saving profile...",
            detail: None,
        }
    }

    /// Completed save status (`tone: Success`).
    #[must_use]
    pub const fn saved() -> Self {
        Self {
            tone: AutosaveTone::Success,
            label: "Profile saved",
            detail: None,
        }
    }

    /// Warning status (`tone: Warning`).
    #[must_use]
    pub const fn warning(label: &'static str, detail: Option<String>) -> Self {
        Self {
            tone: AutosaveTone::Warning,
            label,
            detail,
        }
    }

    /// Error status (`tone: Error`).
    #[must_use]
    pub const fn error(label: &'static str, detail: Option<String>) -> Self {
        Self {
            tone: AutosaveTone::Error,
            label,
            detail,
        }
    }

    /// External conflict warning (`tone: Warning`).
    #[must_use]
    pub const fn conflict() -> Self {
        Self::warning("External profile change", None)
    }

    /// Whether the status chip should currently be visible
    /// (`useAutoSaveChip.ts:43`).
    #[must_use]
    pub const fn is_chip_visible(&self) -> bool {
        !matches!(self.tone, AutosaveTone::Idle)
    }
}

impl Default for AutosaveStatus {
    fn default() -> Self {
        Self::idle()
    }
}

/// Combines multiple autosave statuses using Classic `useAutoSaveChip` priority
/// order (`Error > Saving > Warning > Success > Idle`, keeping the first status
/// on a tie).
#[must_use]
pub fn combine_statuses<'a>(
    statuses: impl IntoIterator<Item = &'a AutosaveStatus>,
) -> AutosaveStatus {
    let mut best = AutosaveStatus::idle();
    for status in statuses {
        if status.tone.priority() > best.tone.priority() {
            best = status.clone();
        }
    }
    best
}
